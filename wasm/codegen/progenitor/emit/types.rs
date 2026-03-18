//! Emit wasm-bindgen struct and enum wrappers.

use std::collections::HashSet;

use heck::ToLowerCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::{EnumInfo, FieldInfo, FieldType, StructInfo};

// ── Struct emission ──────────────────────────────────────────────────────────

/// Emit a wrapper struct with getters for a progenitor type.
pub fn emit_struct(s: &StructInfo, enum_names: &HashSet<&str>) -> TokenStream {
    let sdk_name = format_ident!("{}", s.name);
    let wrapper = format_ident!("Wasm{}", s.name);

    // `Symbol` → `TradingSymbol` in JS to avoid shadowing the built-in.
    let js_name = if s.name == "Symbol" {
        "TradingSymbol".to_string()
    } else {
        s.name.clone()
    };

    // Newtype or empty struct → only expose toJSON.
    if s.is_newtype || s.fields.is_empty() {
        return quote! {
            #[wasm_bindgen(js_name = #js_name)]
            pub struct #wrapper(pub(crate) sdk::#sdk_name);

            #[wasm_bindgen(js_class = #js_name)]
            impl #wrapper {
                #[wasm_bindgen(js_name = toJSON)]
                pub fn to_json(&self) -> String {
                    to_json(&self.0)
                }
            }
        };
    }

    let getters: Vec<TokenStream> = s
        .fields
        .iter()
        .map(|f| emit_getter(f, enum_names))
        .collect();

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        pub struct #wrapper(pub(crate) sdk::#sdk_name);

        #[wasm_bindgen(js_class = #js_name)]
        impl #wrapper {
            #[wasm_bindgen(js_name = toJSON)]
            pub fn to_json(&self) -> String {
                to_json(&self.0)
            }

            #(#getters)*
        }
    }
}

/// Emit a single getter method for a struct field.
fn emit_getter(f: &FieldInfo, enum_names: &HashSet<&str>) -> TokenStream {
    let field = format_ident!("{}", f.rust_name);
    let method = format_ident!("{}", f.rust_name);

    // JS property name: use serde rename if present, otherwise camelCase the rust name.
    let js_name = f
        .serde_rename
        .clone()
        .unwrap_or_else(|| f.rust_name.to_lower_camel_case());

    let needs_js_attr = f.rust_name != js_name;

    let (ret_ty, body) = getter_impl(&f.ty, &field, enum_names);

    let attr = if needs_js_attr {
        quote! { #[wasm_bindgen(getter, js_name = #js_name)] }
    } else {
        quote! { #[wasm_bindgen(getter)] }
    };

    quote! {
        #attr
        pub fn #method(&self) -> #ret_ty {
            #body
        }
    }
}

/// Return type + body expression for a getter.
fn getter_impl(
    ty: &FieldType,
    field: &proc_macro2::Ident,
    enums: &HashSet<&str>,
) -> (TokenStream, TokenStream) {
    match ty {
        FieldType::String => (quote! { String }, quote! { self.0.#field.clone() }),
        FieldType::Bool => (quote! { bool }, quote! { self.0.#field }),
        FieldType::I32 => (quote! { i32 }, quote! { self.0.#field as i32 }),
        FieldType::I64 => (quote! { f64 }, quote! { self.0.#field as f64 }),
        FieldType::F64 => (quote! { f64 }, quote! { self.0.#field }),
        FieldType::JsonMap | FieldType::JsonValue => {
            (quote! { String }, quote! { to_json(&self.0.#field) })
        }

        FieldType::Option(inner) => {
            let (inner_ret, _) = inner_return_type(inner, enums);
            let body = option_body(inner, field, enums);
            (quote! { Option<#inner_ret> }, body)
        }

        FieldType::Vec(inner) => vec_getter(inner, field, enums),

        FieldType::Ref(name) => {
            if enums.contains(name.as_str()) {
                (quote! { String }, quote! { format!("{:?}", self.0.#field) })
            } else {
                let w = format_ident!("Wasm{}", name);
                (quote! { #w }, quote! { #w(self.0.#field.clone()) })
            }
        }
    }
}

/// Body expression for an `Option<T>` getter.
fn option_body(
    inner: &FieldType,
    field: &proc_macro2::Ident,
    enums: &HashSet<&str>,
) -> TokenStream {
    match inner {
        FieldType::String => quote! { self.0.#field.clone() },
        FieldType::Bool => quote! { self.0.#field },
        FieldType::I32 => quote! { self.0.#field.map(|v| v as i32) },
        FieldType::I64 => quote! { self.0.#field.map(|v| v as f64) },
        FieldType::F64 => quote! { self.0.#field },
        FieldType::JsonMap | FieldType::JsonValue => {
            quote! { self.0.#field.as_ref().map(|v| to_json(v)) }
        }
        FieldType::Ref(name) => {
            if enums.contains(name.as_str()) {
                quote! { self.0.#field.as_ref().map(|v| format!("{:?}", v)) }
            } else {
                let w = format_ident!("Wasm{}", name);
                quote! { self.0.#field.clone().map(#w) }
            }
        }
        FieldType::Vec(inner_vec) => {
            let map_fn = vec_map(inner_vec, enums);
            quote! { self.0.#field.as_ref().map(|v| v.iter().cloned().#map_fn.collect()) }
        }
        FieldType::Option(_) => quote! { self.0.#field.clone() },
    }
}

/// Return type + body for a `Vec<T>` getter.
fn vec_getter(
    inner: &FieldType,
    field: &proc_macro2::Ident,
    enums: &HashSet<&str>,
) -> (TokenStream, TokenStream) {
    // Nested Vec<Vec<T>> and Vec<serde_json::Value> can't cross wasm-bindgen.
    // Serialize to JSON string instead.
    match inner {
        FieldType::Vec(_) | FieldType::JsonValue | FieldType::JsonMap => {
            return (quote! { String }, quote! { to_json(&self.0.#field) });
        }
        _ => {}
    }

    let (inner_ret, _) = inner_return_type(inner, enums);
    let map_fn = vec_map(inner, enums);
    (
        quote! { Vec<#inner_ret> },
        quote! { self.0.#field.iter().cloned().#map_fn.collect() },
    )
}

/// Map function for iterating over Vec elements.
fn vec_map(inner: &FieldType, enums: &HashSet<&str>) -> TokenStream {
    match inner {
        FieldType::String | FieldType::Bool | FieldType::F64 => quote! { map(|v| v) },
        FieldType::I32 => quote! { map(|v| v as i32) },
        FieldType::I64 => quote! { map(|v| v as f64) },
        FieldType::JsonMap | FieldType::JsonValue => quote! { map(|v| to_json(&v)) },
        FieldType::Ref(name) => {
            if enums.contains(name.as_str()) {
                quote! { map(|v| format!("{:?}", v)) }
            } else {
                let w = format_ident!("Wasm{}", name);
                quote! { map(#w) }
            }
        }
        _ => quote! { map(|v| to_json(&v)) },
    }
}

/// The wasm-bindgen-compatible return type for an inner type.
fn inner_return_type(ty: &FieldType, enums: &HashSet<&str>) -> (TokenStream, ()) {
    let t = match ty {
        FieldType::String => quote! { String },
        FieldType::Bool => quote! { bool },
        FieldType::I32 => quote! { i32 },
        FieldType::I64 | FieldType::F64 => quote! { f64 },
        FieldType::JsonMap | FieldType::JsonValue => quote! { String },
        FieldType::Ref(name) => {
            if enums.contains(name.as_str()) {
                quote! { String }
            } else {
                let w = format_ident!("Wasm{}", name);
                quote! { #w }
            }
        }
        FieldType::Vec(inner) => {
            let (inner_ret, _) = inner_return_type(inner, enums);
            quote! { Vec<#inner_ret> }
        }
        FieldType::Option(inner) => {
            let (inner_ret, _) = inner_return_type(inner, enums);
            quote! { Option<#inner_ret> }
        }
    };
    (t, ())
}

// ── Enum emission ────────────────────────────────────────────────────────────

/// Emit a wasm-bindgen C-style enum wrapper.
pub fn emit_enum(e: &EnumInfo) -> TokenStream {
    let sdk_name = format_ident!("{}", e.name);
    let wrapper = format_ident!("Wasm{}", e.name);
    let js_name = &e.name;

    let variants: Vec<_> = e.variants.iter().map(|v| format_ident!("{}", v)).collect();
    let indices: Vec<_> = (0..e.variants.len()).map(|i| i as isize).collect();

    let arms: Vec<TokenStream> = variants
        .iter()
        .map(|v| quote! { #wrapper::#v => sdk::#sdk_name::#v })
        .collect();

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        #[derive(Clone, Copy)]
        pub enum #wrapper {
            #(#variants = #indices,)*
        }

        impl #wrapper {
            pub fn into_domain(self) -> sdk::#sdk_name {
                match self {
                    #(#arms,)*
                }
            }
        }
    }
}
