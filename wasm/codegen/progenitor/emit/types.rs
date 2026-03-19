//! Emit wasm-bindgen struct and enum wrappers.

use std::collections::HashSet;

use heck::ToLowerCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::{EnumDetails, FieldDetails, StructDetails};
use super::type_map;

// ── Struct emission ──────────────────────────────────────────────────────────

/// Emit a wrapper struct with getters for a progenitor type.
pub fn emit_struct(s: &StructDetails, enum_names: &HashSet<&str>) -> TokenStream {
    let sdk_name = format_ident!("{}", s.name);
    let wrapper = format_ident!("Wasm{}", s.name);
    let js_name = type_map::js_name(&s.name);

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
fn emit_getter(f: &FieldDetails, enum_names: &HashSet<&str>) -> TokenStream {
    let field = format_ident!("{}", f.name);
    let method = format_ident!("{}", f.name);

    // JS property name: use serde rename if present, otherwise camelCase the rust name.
    let js_name = f
        .serde_rename
        .clone()
        .unwrap_or_else(|| f.name.to_lower_camel_case());

    let needs_js_attr = f.name != js_name;

    let (ret_ty, body) = type_map::getter_mapping(&f.ty, &field, enum_names);

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
    let sdk_name = format_ident!("{}", e.name);
    let wrapper = format_ident!("Wasm{}", e.name);
    let js_name = &e.name;

    let variants: Vec<_> = e
        .variants
        .iter()
        .map(|v| format_ident!("{}", v.name))
        .collect();
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
