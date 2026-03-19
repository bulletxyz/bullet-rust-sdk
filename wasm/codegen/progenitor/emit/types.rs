//! Emit wasm-bindgen struct and enum wrappers.

use std::collections::HashSet;

use heck::ToLowerCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::{EnumDetails, FieldDetails, FieldKind, StructDetails};
use super::type_map;

// ── SDK Path Helper ──────────────────────────────────────────────────────────

/// Build a fully-qualified SDK path from a module path.
///
/// Module paths are relative to the progenitor codegen root (e.g., `["types", "error"]`).
/// We emit `bullet_rust_sdk::codegen::<module_path>::<name>`.
fn sdk_qualified_path(module_path: &[String], name: &str) -> TokenStream {
    let name_ident = format_ident!("{}", name);

    // Build the full path by chaining segments with ::
    let mut tokens = quote! { bullet_rust_sdk::codegen };
    for seg in module_path {
        let seg_ident = format_ident!("{}", seg);
        tokens = quote! { #tokens::#seg_ident };
    }
    quote! { #tokens::#name_ident }
}

/// Check if derives contain Serialize.
fn has_serialize(derives: &[String]) -> bool {
    derives
        .iter()
        .any(|d| d == "Serialize" || d.ends_with("::Serialize"))
}

// ── SDK Path Helper ──────────────────────────────────────────────────────────

/// Build a fully-qualified SDK path from a module path.
///
/// Module paths are relative to the progenitor codegen root (e.g., `["types", "error"]`).
/// We emit `bullet_rust_sdk::codegen::<module_path>::<name>`.
fn sdk_qualified_path(module_path: &[String], name: &str) -> TokenStream {
    let name_ident = format_ident!("{}", name);

    // Build the full path by chaining segments with ::
    let mut tokens = quote! { bullet_rust_sdk::codegen };
    for seg in module_path {
        let seg_ident = format_ident!("{}", seg);
        tokens = quote! { #tokens::#seg_ident };
    }
    quote! { #tokens::#name_ident }
}

/// Check if derives contain Serialize.
fn has_serialize(derives: &[String]) -> bool {
    derives.iter().any(|d| d.contains("Serialize"))
}

// ── Struct emission ──────────────────────────────────────────────────────────

/// Emit a wrapper struct with getters for a progenitor type.
pub fn emit_struct(s: &StructDetails, enum_names: &HashSet<&str>) -> TokenStream {
    let sdk_type = sdk_qualified_path(&s.module_path, &s.name);
    let wrapper = format_ident!("Wasm{}", s.name);
    let js_name = type_map::js_name(&s.name);
    let serializable = has_serialize(&s.derives);

    // Newtype or empty struct → only expose toJSON (if serializable).
    if s.is_newtype || s.fields.is_empty() {
        let to_json_method = if serializable {
            quote! {
                #[wasm_bindgen(js_name = toJSON)]
                pub fn to_json(&self) -> String {
                    to_json(&self.0)
                }
            }
        } else {
            quote! {}
        };

        return quote! {
            #[wasm_bindgen(js_name = #js_name)]
            pub struct #wrapper(pub(crate) #sdk_type);

            #[wasm_bindgen(js_class = #js_name)]
            impl #wrapper {
                #to_json_method
            }
        };
    }

    let getters: Vec<TokenStream> = s
        .fields
        .iter()
        .map(|f| emit_getter(f, enum_names))
        .collect();

    let to_json_method = if serializable {
        quote! {
            #[wasm_bindgen(js_name = toJSON)]
            pub fn to_json(&self) -> String {
                to_json(&self.0)
            }
        }
    } else {
        quote! {}
    };

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        pub struct #wrapper(pub(crate) #sdk_type);

        #[wasm_bindgen(js_class = #js_name)]
        impl #wrapper {
            #to_json_method

            #(#getters)*
        }
    }
}

/// Emit a single getter method for a struct field.
fn emit_getter(f: &FieldDetails, enum_names: &HashSet<&str>) -> TokenStream {
    // Determine method name, JS property name, and field accessor based on field kind.
    let (method, js_name, field_accessor): (_, String, TokenStream) = match &f.kind {
        FieldKind::Named(name) => {
            let method = format_ident!("{}", name);
            let js_name = f
                .serde_rename
                .clone()
                .unwrap_or_else(|| name.to_lower_camel_case());
            let accessor = quote! { #method };
            (method, js_name, accessor)
        }
        FieldKind::Index(i) => {
            let method = format_ident!("field_{}", i);
            let js_name = format!("field{}", i);
            let index = syn::Index::from(*i);
            let accessor = quote! { #index };
            (method, js_name, accessor)
        }
    };

    let method_str = method.to_string();
    let needs_js_attr = method_str != js_name;

    let (ret_ty, body) = type_map::getter_mapping(&f.ty, &field_accessor, enum_names);

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

// ── Enum emission ────────────────────────────────────────────────────────────

/// Emit a wasm-bindgen C-style enum wrapper.
pub fn emit_enum(e: &EnumDetails) -> TokenStream {
    let sdk_type = sdk_qualified_path(&e.module_path, &e.name);
    let wrapper = format_ident!("Wasm{}", e.name);
    let js_name = type_map::js_name(&e.name);

    let variants: Vec<_> = e
        .variants
        .iter()
        .map(|v| format_ident!("{}", v.name))
        .collect();
    let indices: Vec<_> = (0..e.variants.len()).map(|i| i as isize).collect();

    let arms: Vec<TokenStream> = variants
        .iter()
        .map(|v| quote! { #wrapper::#v => #sdk_type::#v })
        .collect();

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        #[derive(Clone, Copy)]
        pub enum #wrapper {
            #(#variants = #indices,)*
        }

        impl #wrapper {
            pub fn into_domain(self) -> #sdk_type {
                match self {
                    #(#arms,)*
                }
            }
        }
    }
}
