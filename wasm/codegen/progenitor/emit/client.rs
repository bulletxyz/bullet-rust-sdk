//! Emit wasm-bindgen client method wrappers.

use std::collections::HashSet;

use heck::ToLowerCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::MethodDetails;
use super::type_map;

/// Methods to skip when generating WASM bindings.
const SKIP_METHODS: &[&str] = &["new", "new_with_client", "api_version"];

/// Emit the full `impl WasmTradingApi` block.
pub fn emit_client_impl(methods: &[&MethodDetails], _enum_names: &HashSet<&str>) -> TokenStream {
    let method_tokens: Vec<TokenStream> = methods
        .iter()
        .filter(|m| m.is_async && !SKIP_METHODS.contains(&m.name.as_str()))
        .map(|m| emit_method(m))
        .collect();

    quote! {
        use crate::client::WasmTradingApi;
        use crate::errors::WasmResult;

        #[wasm_bindgen(js_class = Client)]
        impl WasmTradingApi {
            #(#method_tokens)*
        }
    }
}

fn emit_method(m: &MethodDetails) -> TokenStream {
    let method = format_ident!("{}", m.name);
    let js_name = m.name.to_lower_camel_case();

    // Build the WASM-facing parameter list and call arguments.
    let (wasm_params, call_args): (Vec<TokenStream>, Vec<TokenStream>) = m
        .params
        .iter()
        .map(|p| {
            let name = format_ident!("{}", p.name);
            let (param_ty, call_arg) = type_map::param_mapping(&p.ty, &name);
            (quote! { #name: #param_ty }, call_arg)
        })
        .unzip();

    // Return type + response handling.
    let (ret_ty, body) = match &m.return_type {
        Some(ty) => type_map::return_mapping(ty, &method, &call_args),
        None => (
            quote! { WasmResult<()> },
            quote! {
                self.inner.#method(#(#call_args),*).await?;
                Ok(())
            },
        ),
    };

    let js_attr = if m.name != js_name {
        quote! { #[wasm_bindgen(js_name = #js_name)] }
    } else {
        quote! {}
    };

    quote! {
        #js_attr
        pub async fn #method(&self, #(#wasm_params),*) -> #ret_ty {
            #body
        }
    }
}
