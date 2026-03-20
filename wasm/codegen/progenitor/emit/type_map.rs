//! Type mapping from `RustType` to WASM-bindgen types.
//!
//! This is **THE** single file to update when adding support for a new type.
//! All type mappings are consolidated here for easy maintenance.
//!
//! # Adding a new type
//!
//! 1. If it's a primitive or common pattern, add it to the appropriate match arm.
//! 2. If it's a `Named` type (like `ByteStream`), match on the name string.
//! 3. Run `cargo check` on the wasm crate to verify it compiles.

use std::collections::HashSet;

use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};

use super::super::{Primitive, RustType};

// ── JS Name Renames ──────────────────────────────────────────────────────────

/// Types that need JS name remapping to avoid shadowing built-ins.
///
/// Add entries here when a Rust type name conflicts with a JS global.
pub const JS_RENAMES: &[(&str, &str)] = &[
    ("Symbol", "TradingSymbol"), // Avoid shadowing JS Symbol
];

/// Get the JS-facing name for a type, applying renames if needed.
pub fn js_name(rust_name: &str) -> String {
    for (from, to) in JS_RENAMES {
        if rust_name == *from {
            return (*to).to_string();
        }
    }
    rust_name.to_string()
}

// ── Core Type Helpers ────────────────────────────────────────────────────────

/// Map a `RustType` to its wasm-bindgen-compatible type tokens.
///
/// This is the single source of truth for Rust→WASM type mapping.
/// wasm-bindgen only supports i32/f64 for numbers (JS numbers are IEEE 754 doubles),
/// so ≤32-bit integers → i32 and 64-bit/floats → f64.
fn wasm_type(ty: &RustType, enums: &HashSet<&str>) -> TokenStream {
    match ty {
        RustType::String => quote! { String },
        RustType::Bool => quote! { bool },
        RustType::Primitive(
            Primitive::I8
            | Primitive::I16
            | Primitive::I32
            | Primitive::U8
            | Primitive::U16
            | Primitive::U32,
        ) => quote! { i32 },
        RustType::Primitive(Primitive::I64 | Primitive::U64 | Primitive::F32 | Primitive::F64) => {
            quote! { f64 }
        }
        RustType::Map(_, _) => quote! { String },
        RustType::Named { name, .. } if name == "Value" => quote! { String },
        RustType::Named { name, .. } if enums.contains(name.as_str()) => quote! { String },
        RustType::Named { name, .. } => {
            let w = format_ident!("Wasm{}", name);
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
/// `expr` is the tokens for accessing the value (e.g., `self.0.field`, `v`).
/// Returns tokens for the converted expression.
fn value_conversion(ty: &RustType, expr: &TokenStream, enums: &HashSet<&str>) -> TokenStream {
    match ty {
        RustType::String => quote! { #expr.clone() },
        RustType::Bool => quote! { #expr },
        RustType::Primitive(
            Primitive::I8
            | Primitive::I16
            | Primitive::I32
            | Primitive::U8
            | Primitive::U16
            | Primitive::U32,
        ) => quote! { #expr as i32 },
        RustType::Primitive(Primitive::I64 | Primitive::U64 | Primitive::F32 | Primitive::F64) => {
            quote! { #expr as f64 }
        }
        RustType::Map(_, _) => quote! { to_json(&#expr) },
        RustType::Named { name, .. } if name == "Value" => quote! { to_json(&#expr) },
        RustType::Named { name, .. } if enums.contains(name.as_str()) => {
            quote! { #expr.to_string() }
        }
        RustType::Named { name, .. } => {
            let w = format_ident!("Wasm{}", name);
            quote! { #w(#expr.clone()) }
        }
        _ => quote! { to_json(&#expr) },
    }
}

// ── Getter Mapping ───────────────────────────────────────────────────────────

/// Map a struct field type to its WASM getter return type and body.
///
/// # Arguments
/// - `ty`: The field's `RustType`
/// - `field`: The field accessor tokens (e.g., `field_name` or `0` for tuple structs)
/// - `enums`: Set of enum names (to distinguish enum refs from struct refs)
///
/// # Returns
/// `(return_type, body)` tokens for the getter method.
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
        // Copy types can pass through directly.
        RustType::Bool => quote! { self.0.#field },
        RustType::Primitive(Primitive::F64) => quote! { self.0.#field },
        RustType::Primitive(Primitive::F32) => quote! { self.0.#field.map(|v| v as f64) },
        // String/clone types can clone through directly.
        RustType::String => quote! { self.0.#field.clone() },
        RustType::Option(_) => quote! { self.0.#field.clone() },
        // Vec inside Option needs ref access.
        RustType::Vec(inner_vec) => {
            let conv = value_conversion(inner_vec, &quote! { v.clone() }, enums);
            quote! { self.0.#field.as_ref().map(|v| v.iter().map(|v| #conv).collect()) }
        }
        // Everything else: map the inner value through conversion.
        _ => {
            let conv = value_conversion(inner, &quote! { v }, enums);
            // Named structs/maps/Value need .as_ref() to avoid moving out of the Option.
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
    // Nested Vec<Vec<T>>, Vec<Map>, and Vec<serde_json::Value> can't cross
    // wasm-bindgen boundaries. Serialize to JSON string instead.
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
    let conv = value_conversion(inner, &quote! { v }, enums);

    (
        quote! { Vec<#inner_ret> },
        quote! { self.0.#field.iter().cloned().map(|v| #conv).collect() },
    )
}

// ── Parameter Mapping ────────────────────────────────────────────────────────

/// Map a primitive to its wasm-bindgen parameter type.
///
/// Differs from getter mapping: params use `i64` for 64-bit integers (BigInt support)
/// instead of `f64`.
fn wasm_param_primitive(p: &Primitive) -> TokenStream {
    match p {
        Primitive::I8
        | Primitive::I16
        | Primitive::I32
        | Primitive::U8
        | Primitive::U16
        | Primitive::U32 => quote! { i32 },
        Primitive::I64 | Primitive::U64 => quote! { i64 },
        Primitive::F32 | Primitive::F64 => quote! { f64 },
    }
}

/// Map a method parameter type to its WASM declaration type and call argument expression.
///
/// # Arguments
/// - `ty`: The parameter's `RustType`
/// - `name`: The parameter name as an identifier
///
/// # Returns
/// `(param_type, call_arg)` tokens for the method signature and body.
pub fn param_mapping(ty: &RustType, name: &Ident) -> (TokenStream, TokenStream) {
    match ty {
        // &str → passes through
        RustType::Ref(inner) if matches!(inner.as_ref(), RustType::String) => {
            (quote! { &str }, quote! { #name })
        }

        // &types::Foo → &WasmFoo, unwrap to &inner.0
        RustType::Ref(inner) => match inner.as_ref() {
            RustType::Named { name: ty_name, .. } => {
                let w = format_ident!("Wasm{}", ty_name);
                (quote! { &#w }, quote! { &#name.0 })
            }
            // &[types::Foo] → js_sys::Array, extract and convert
            RustType::Slice(elem) => match elem.as_ref() {
                RustType::Named { name: ty_name, .. } => {
                    let w = format_ident!("Wasm{}", ty_name);
                    (
                        quote! { js_sys::Array },
                        quote! { &extract_array::<#w>(#name)?.into_iter().map(|w| w.0.clone()).collect::<Vec<_>>() },
                    )
                }
                _ => (quote! { &str }, quote! { #name }), // fallback
            },
            _ => (quote! { &str }, quote! { #name }), // fallback
        },

        // Option<&str> → Option<String>, convert via as_deref()
        RustType::Option(inner) if matches!(inner.as_ref(), RustType::Ref(r) if matches!(r.as_ref(), RustType::String)) => {
            (quote! { Option<String> }, quote! { #name.as_deref() })
        }

        // Option<primitive> → Option<wasm_primitive>
        RustType::Option(inner) if matches!(inner.as_ref(), RustType::Primitive(_)) => {
            let RustType::Primitive(p) = inner.as_ref() else {
                unreachable!()
            };
            let wasm_ty = wasm_param_primitive(p);
            (quote! { Option<#wasm_ty> }, quote! { #name })
        }

        // Primitives
        RustType::Primitive(p) => {
            let wasm_ty = wasm_param_primitive(p);
            (quote! { #wasm_ty }, quote! { #name })
        }

        // Fallback: pass as-is (shouldn't happen for well-formed progenitor output)
        _ => (quote! { JsValue }, quote! { #name }),
    }
}

// ── Return Mapping ───────────────────────────────────────────────────────────

/// Map a method return type to its WASM return type and body.
///
/// This expects the return type to be `RustType::ResponseValue(T)` as produced by walk.
///
/// # Arguments
/// - `ty`: The method's return `RustType` (should be `ResponseValue(...)`)
/// - `method`: The method name as an identifier
/// - `call_args`: The call argument expressions
///
/// # Returns
/// `(return_type, body)` tokens for the method.
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
            // Shouldn't happen for progenitor output, but handle gracefully
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
        // ResponseValue<()>
        RustType::Tuple(elems) if elems.is_empty() => (
            quote! { WasmResult<()> },
            quote! {
                self.inner.#method(#(#call_args),*).await?;
                Ok(())
            },
        ),

        // ResponseValue<Vec<types::Foo>>
        RustType::Vec(elem) => match elem.as_ref() {
            RustType::Named { name, .. } => {
                let w = format_ident!("Wasm{}", name);
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

        // ResponseValue<Map<...>>
        RustType::Map(_, _) => (
            quote! { WasmResult<String> },
            quote! {
                Ok(serde_json::to_string(&self.inner.#method(#(#call_args),*).await?.into_inner())?)
            },
        ),

        // ResponseValue<types::Foo> (including ByteStream as Named)
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

            // Regular struct
            let w = format_ident!("Wasm{}", name);
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
