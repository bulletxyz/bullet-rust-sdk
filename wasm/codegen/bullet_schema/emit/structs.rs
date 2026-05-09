//! Emit wasm-bindgen struct wrappers with typed constructors.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::SchemaStruct;
use super::field_assignments;

pub fn emit_struct(s: &SchemaStruct) -> TokenStream {
    let type_name = format_ident!("{}", s.type_name);
    let wrapper_name = format_ident!("Wasm{}", s.type_name);
    let js_name = &s.type_name;

    // Some types are generic over Address.
    let inner_type_decl: TokenStream = match s.type_name.as_str() {
        "CreateVaultArgs" | "UpdateGlobalConfigArgs" | "UpdateGlobalConfigArgsV1" => {
            quote! { #type_name<Address> }
        }
        _ => quote! { #type_name },
    };

    // Sort: required params first, optional last.
    let mut field_order: Vec<usize> = (0..s.fields.len()).collect();
    field_order.sort_by_key(|&i| s.fields[i].is_optional as u8);

    let params: Vec<TokenStream> = field_order
        .iter()
        .map(|&i| {
            let name = format_ident!("{}", s.fields[i].name);
            let ty: TokenStream = s.fields[i]
                .param_type
                .parse()
                .expect("param type should parse");
            quote! { #name: #ty }
        })
        .collect();

    let assignments = field_assignments(&s.fields);

    quote! {
        #[doc = concat!("Wrapper for `", stringify!(#type_name), "`.")]
        #[wasm_bindgen(js_name = #js_name)]
        pub struct #wrapper_name {
            pub(crate) inner: #inner_type_decl,
        }

        #[wasm_bindgen(js_class = #js_name)]
        impl #wrapper_name {
            #[wasm_bindgen(constructor)]
            #[allow(clippy::too_many_arguments)]
            pub fn new(#(#params),*) -> WasmResult<#wrapper_name> {
                Ok(#wrapper_name {
                    inner: #type_name {
                        #(#assignments),*
                    },
                })
            }
        }
    }
}
