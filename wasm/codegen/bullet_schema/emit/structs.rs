//! Emit wasm-bindgen struct wrappers with typed constructors.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::SchemaStruct;
use super::{emitted_params, field_assignments};

pub fn emit_struct(s: &SchemaStruct) -> TokenStream {
    let type_name = format_ident!("{}", s.type_name);
    let wrapper_name = format_ident!("Wasm{}", s.type_name);
    let js_name = &s.type_name;

    // Some types are generic over Address.
    let inner_type_decl: TokenStream = match s.type_name.as_str() {
        "CreateVaultArgs"
        | "UpdateGlobalConfigArgs"
        | "UpdateGlobalConfigArgsV1"
        | "UpdateUserMarginDiscountArgs"
        | "TradingCreditsArgs" => {
            quote! { #type_name<Address> }
        }
        _ => quote! { #type_name },
    };

    let params = emitted_params(&s.fields);
    let param_docs = params.jsdoc;
    let param_tokens = params.tokens;
    let assignments = field_assignments(&s.fields);
    let summary = format!("Create a `{}`.", s.type_name);
    let returns_doc = format!("@returns {{{}}}", s.type_name);

    quote! {
        #[doc = concat!("Wrapper for `", stringify!(#type_name), "`.")]
        #[wasm_bindgen(js_name = #js_name)]
        pub struct #wrapper_name {
            pub(crate) inner: #inner_type_decl,
        }

        #[wasm_bindgen(js_class = #js_name)]
        impl #wrapper_name {
            #[doc = #summary]
            #(#[doc = #param_docs])*
            #[doc = #returns_doc]
            #[wasm_bindgen(constructor)]
            #[allow(clippy::too_many_arguments)]
            pub fn new(#(#param_tokens),*) -> WasmResult<#wrapper_name> {
                Ok(#wrapper_name {
                    inner: #type_name {
                        #(#assignments),*
                    },
                })
            }
        }
    }
}
