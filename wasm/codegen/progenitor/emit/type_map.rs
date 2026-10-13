//! `RustType` -> wasm-bindgen type mapping. The single place to add support for a new type.

use std::collections::HashSet;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use super::super::{Primitive, RustType};

/// JS renames for progenitor types whose names clash with a JS global or a CallMessage codegen
/// type. Both generators share one JS namespace, so a clash is a duplicate `__wbg_<name>_free`
/// link error rather than a Rust error.
pub const JS_RENAMES: &[(&str, &str)] = &[
    ("Symbol", "TradingSymbol"), // Avoid shadowing JS Symbol
    // Also a CallMessage argument type; this is the API's read-side view.
    ("TransferEndpoint", "TransferEndpointInfo"),
    // `Vault` is the CallMessage namespace for vault leader factories.
    ("Vault", "VaultInfo"),
];

pub fn js_name(rust_name: &str) -> String {
    for (from, to) in JS_RENAMES {
        if rust_name == *from {
            return (*to).to_string();
        }
    }
    rust_name.to_string()
}

/// Wrapper ident, derived from [`js_name`] so renamed types also avoid an ambiguous glob
/// re-export in `lib.rs`.
pub fn wrapper_ident(rust_name: &str) -> Ident {
    format_ident!("Wasm{}", js_name(rust_name))
}

/// Map a `RustType` to its TypeScript/JSDoc type string.
pub fn js_type_string(ty: &RustType, enums: &HashSet<&str>) -> String {
    match ty {
        RustType::String => "string".to_string(),
        RustType::Bool => "boolean".to_string(),
        RustType::Primitive(_) => "number".to_string(),
        RustType::Map(_, _) => "object".to_string(),
        RustType::Named { name, .. } if name == "Value" => "object".to_string(),
        RustType::Named { name, .. } if name == "ByteStream" => "string".to_string(),
        RustType::Named { name, .. } if enums.contains(name.as_str()) => "string".to_string(),
        RustType::Named { name, .. } => js_name(name),
        RustType::Option(inner) => {
            format!("{} | undefined", js_type_string(inner, enums))
        }
        RustType::Vec(inner) => {
            format!("Array<{}>", js_type_string(inner, enums))
        }
        RustType::Ref(inner) => match inner.as_ref() {
            RustType::String => "string".to_string(),
            RustType::Slice(elem) => format!("Array<{}>", js_type_string(elem, enums)),
            other => js_type_string(other, enums),
        },
        RustType::ResponseValue(inner) => js_type_string(inner, enums),
        RustType::Tuple(elems) if elems.is_empty() => "void".to_string(),
        _ => "any".to_string(),
    }
}

/// Map a method parameter's `RustType` to its JSDoc type string.
pub fn param_js_type(ty: &RustType, enums: &HashSet<&str>) -> String {
    match ty {
        RustType::Option(inner) if matches!(inner.as_ref(), RustType::Ref(r) if matches!(r.as_ref(), RustType::String)) => {
            "string".to_string()
        }
        _ => js_type_string(ty, enums),
    }
}

/// Map a method return `RustType` to its JSDoc `@returns` string.
pub fn return_js_type(ty: &RustType, enums: &HashSet<&str>) -> String {
    let inner = js_type_string(ty, enums);
    format!("Promise<{inner}>")
}

/// Map a `Primitive` to its Rust type tokens.
fn primitive_type(p: &Primitive) -> TokenStream {
    match p {
        Primitive::I8 => quote! { i8 },
        Primitive::I16 => quote! { i16 },
        Primitive::I32 => quote! { i32 },
        Primitive::U8 => quote! { u8 },
        Primitive::U16 => quote! { u16 },
        Primitive::U32 => quote! { u32 },
        Primitive::I64 => quote! { i64 },
        Primitive::U64 => quote! { u64 },
        Primitive::F32 => quote! { f32 },
        Primitive::F64 => quote! { f64 },
    }
}

/// Map a `RustType` to its wasm-bindgen-compatible type tokens.
fn wasm_type(ty: &RustType, enums: &HashSet<&str>) -> TokenStream {
    match ty {
        RustType::String => quote! { String },
        RustType::Bool => quote! { bool },
        RustType::Primitive(p) => primitive_type(p),
        RustType::Map(_, _) => quote! { String },
        RustType::Named { name, .. } if name == "Value" => quote! { String },
        RustType::Named { name, .. } if enums.contains(name.as_str()) => quote! { String },
        RustType::Named { name, .. } => {
            let w = wrapper_ident(name);
            quote! { #w }
        }
        RustType::Option(inner) => {
            let inner_ty = wasm_type(inner, enums);
            quote! { Option<#inner_ty> }
        }
        RustType::Vec(inner) => {
            let inner_ty = wasm_type(inner, enums);
            quote! { Vec<#inner_ty> }
        }
        _ => quote! { String },
    }
}

/// Convert a value expression from a Rust type to its WASM representation.
///
/// `expr` accesses the value (e.g. `self.0.field`, `v`).
fn value_conversion(ty: &RustType, expr: &TokenStream, enums: &HashSet<&str>) -> TokenStream {
    match ty {
        RustType::String => quote! { #expr.clone() },
        RustType::Bool | RustType::Primitive(_) => quote! { #expr },
        RustType::Map(_, _) => quote! { to_json(&#expr) },
        RustType::Named { name, .. } if name == "Value" => quote! { to_json(&#expr) },
        RustType::Named { name, .. } if enums.contains(name.as_str()) => {
            quote! { #expr.to_string() }
        }
        RustType::Named { name, .. } => {
            let w = wrapper_ident(name);
            quote! { #w(#expr.clone()) }
        }
        _ => quote! { to_json(&#expr) },
    }
}

/// Map a struct field type to its WASM getter return type and body.
/// Returns `(return_type, body)`. `enums` distinguishes enum refs from struct refs.
pub fn getter_mapping(
    ty: &RustType,
    field: &TokenStream,
    enums: &HashSet<&str>,
) -> (TokenStream, TokenStream) {
    match ty {
        RustType::Option(inner) => option_getter(inner, field, enums),
        RustType::Vec(inner) => vec_getter(inner, field, enums),
        _ => {
            let ret = wasm_type(ty, enums);
            let expr = quote! { self.0.#field };
            let body = value_conversion(ty, &expr, enums);
            (ret, body)
        }
    }
}

/// Map `Option<T>` to getter return type and body.
fn option_getter(
    inner: &RustType,
    field: &TokenStream,
    enums: &HashSet<&str>,
) -> (TokenStream, TokenStream) {
    let inner_ret = wasm_type(inner, enums);

    let body = match inner {
        RustType::Bool | RustType::Primitive(_) => quote! { self.0.#field },
        RustType::String => quote! { self.0.#field.clone() },
        RustType::Option(_) => quote! { self.0.#field.clone() },
        RustType::Vec(inner_vec) => {
            let conv = value_conversion(inner_vec, &quote! { v.clone() }, enums);
            quote! { self.0.#field.as_ref().map(|v| v.iter().map(|v| #conv).collect()) }
        }
        _ => {
            let conv = value_conversion(inner, &quote! { v }, enums);
            // `.as_ref()` avoids moving out of the Option.
            let needs_ref = matches!(inner, RustType::Map(_, _) | RustType::Named { .. });
            if needs_ref {
                quote! { self.0.#field.as_ref().map(|v| #conv) }
            } else {
                quote! { self.0.#field.map(|v| #conv) }
            }
        }
    };

    (quote! { Option<#inner_ret> }, body)
}

/// Map `Vec<T>` to getter return type and body.
fn vec_getter(
    inner: &RustType,
    field: &TokenStream,
    enums: &HashSet<&str>,
) -> (TokenStream, TokenStream) {
    // Nested Vec, Vec<Map> and Vec<Value> can't cross the wasm-bindgen boundary: use JSON.
    match inner {
        RustType::Vec(_) | RustType::Map(_, _) => {
            return (quote! { String }, quote! { to_json(&self.0.#field) });
        }
        RustType::Named { name, .. } if name == "Value" => {
            return (quote! { String }, quote! { to_json(&self.0.#field) });
        }
        _ => {}
    }

    let inner_ret = wasm_type(inner, enums);

    let body = match inner {
        RustType::Bool | RustType::Primitive(_) | RustType::String => {
            quote! { self.0.#field.to_vec() }
        }
        _ => {
            let conv = value_conversion(inner, &quote! { v }, enums);
            quote! { self.0.#field.iter().map(|v| #conv).collect() }
        }
    };

    (quote! { Vec<#inner_ret> }, body)
}

/// Map a method parameter type to its WASM declaration type and call argument expression.
/// Returns `(param_type, call_arg)`.
pub fn param_mapping(ty: &RustType, name: &Ident) -> (TokenStream, TokenStream) {
    match ty {
        RustType::Ref(inner) if matches!(inner.as_ref(), RustType::String) => {
            (quote! { &str }, quote! { #name })
        }

        RustType::Ref(inner) => match inner.as_ref() {
            RustType::Named { name: ty_name, .. } => {
                let w = wrapper_ident(ty_name);
                (quote! { &#w }, quote! { &#name.0 })
            }
            RustType::Slice(elem) => match elem.as_ref() {
                RustType::Named { name: ty_name, .. } => {
                    let w = wrapper_ident(ty_name);
                    (
                        quote! { js_sys::Array },
                        quote! { &extract_array::<#w>(#name)?.into_iter().map(|w| w.0.clone()).collect::<Vec<_>>() },
                    )
                }
                _ => (quote! { &str }, quote! { #name }), // fallback
            },
            _ => (quote! { &str }, quote! { #name }), // fallback
        },

        RustType::Option(inner) if matches!(inner.as_ref(), RustType::Ref(r) if matches!(r.as_ref(), RustType::String)) => {
            (quote! { Option<String> }, quote! { #name.as_deref() })
        }

        RustType::Option(inner) if matches!(inner.as_ref(), RustType::Primitive(_)) => {
            let RustType::Primitive(p) = inner.as_ref() else {
                unreachable!()
            };
            let wasm_ty = primitive_type(p);
            (quote! { Option<#wasm_ty> }, quote! { #name })
        }

        RustType::Option(inner) if matches!(inner.as_ref(), RustType::Bool) => {
            (quote! { Option<bool> }, quote! { #name })
        }

        RustType::Primitive(p) => {
            let wasm_ty = primitive_type(p);
            (quote! { #wasm_ty }, quote! { #name })
        }

        RustType::Bool => (quote! { bool }, quote! { #name }),

        // Not expected for well-formed progenitor output.
        _ => (quote! { JsValue }, quote! { #name }),
    }
}

/// Map a method return type to its WASM return type and body.
/// Expects `RustType::ResponseValue(T)` from walk. Returns `(return_type, body)`.
pub fn return_mapping(
    ty: &RustType,
    method: &Ident,
    call_args: &[TokenStream],
) -> (TokenStream, TokenStream) {
    match ty {
        RustType::ResponseValue(inner) => response_value_mapping(inner, method, call_args),
        RustType::Tuple(elems) if elems.is_empty() => (
            quote! { WasmResult<()> },
            quote! {
                self.inner.#method(#(#call_args),*).await?;
                Ok(())
            },
        ),
        _ => {
            // Not expected for progenitor output.
            (
                quote! { WasmResult<()> },
                quote! {
                    self.inner.#method(#(#call_args),*).await?;
                    Ok(())
                },
            )
        }
    }
}

/// Map the inner type of `ResponseValue<T>`.
fn response_value_mapping(
    inner: &RustType,
    method: &Ident,
    call_args: &[TokenStream],
) -> (TokenStream, TokenStream) {
    match inner {
        RustType::Tuple(elems) if elems.is_empty() => (
            quote! { WasmResult<()> },
            quote! {
                self.inner.#method(#(#call_args),*).await?;
                Ok(())
            },
        ),

        RustType::Vec(elem) => match elem.as_ref() {
            RustType::Named { name, .. } => {
                let w = wrapper_ident(name);
                (
                    quote! { WasmResult<Vec<#w>> },
                    quote! {
                        Ok(self.inner.#method(#(#call_args),*).await?.into_inner()
                            .into_iter().map(#w).collect())
                    },
                )
            }
            _ => json_fallback(method, call_args),
        },

        RustType::Map(_, _) => (
            quote! { WasmResult<String> },
            quote! {
                Ok(serde_json::to_string(&self.inner.#method(#(#call_args),*).await?.into_inner())?)
            },
        ),

        RustType::Named { name, .. } => {
            // ByteStream → read to string
            if name == "ByteStream" {
                return (
                    quote! { WasmResult<String> },
                    quote! {
                        use futures_util::TryStreamExt as _;
                        let bytes: Vec<u8> = self.inner.#method(#(#call_args),*).await?.into_inner().into_inner()
                            .map_ok(|b| b.to_vec())
                            .try_concat()
                            .await?;
                        Ok(String::from_utf8_lossy(&bytes).into_owned())
                    },
                );
            }

            let w = wrapper_ident(name);
            (
                quote! { WasmResult<#w> },
                quote! { Ok(#w(self.inner.#method(#(#call_args),*).await?.into_inner())) },
            )
        }

        _ => json_fallback(method, call_args),
    }
}

fn json_fallback(method: &Ident, call_args: &[TokenStream]) -> (TokenStream, TokenStream) {
    (
        quote! { WasmResult<String> },
        quote! {
            Ok(serde_json::to_string(&self.inner.#method(#(#call_args),*).await?.into_inner())?)
        },
    )
}
