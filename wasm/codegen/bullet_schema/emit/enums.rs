//! Emit wasm-bindgen C-style enum wrappers with `into_domain()` conversion.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::SchemaEnum;

pub fn emit_enum(e: &SchemaEnum) -> TokenStream {
    let type_name = format_ident!("{}", e.type_name);
    let wrapper_name = format_ident!("Wasm{}", e.type_name);
    let js_name = &e.type_name;

    let variant_idents: Vec<_> = e.variants.iter().map(|v| format_ident!("{}", v)).collect();
    let variant_indices: Vec<_> = (0..e.variants.len())
        .map(|i| i as isize)
        .collect::<Vec<_>>();

    let match_arms: Vec<TokenStream> = variant_idents
        .iter()
        .map(|v| {
            quote! { #wrapper_name::#v => #type_name::#v }
        })
        .collect();

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        #[derive(Clone, Copy)]
        pub enum #wrapper_name {
            #(#variant_idents = #variant_indices,)*
        }

        impl #wrapper_name {
            fn into_domain(self) -> #type_name {
                match self {
                    #(#match_arms,)*
                }
            }
        }
    }
}
