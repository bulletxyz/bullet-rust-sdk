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

    ParamMapping {
        param_type,
        conversion,
        is_optional: true,
    }
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
        } else if let Some(func) = single_arg_function(&inner.conversion) {
            // e.g. "ClientOrderId({v})" -> "{v}.map(ClientOrderId)"
            format!("{{v}}.map({func})")
        } else {
            format!("{{v}}.map(|v| {inner_expr})")
        }
    }
}

/// Extract a simple one-argument constructor/function like `TypeName({v})`.
fn single_arg_function(conversion: &str) -> Option<&str> {
    if let Some(rest) = conversion.strip_suffix("({v})") {
        if !rest.is_empty()
            && rest
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == ':')
        {
            Some(rest)
        } else {
            None
        }
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
        format!("{{v}}.as_deref().map(|s| {inner_expr})")
    }
}
