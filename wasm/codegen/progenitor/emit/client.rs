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
pub fn emit_client_impl(methods: &[&MethodDetails], enum_names: &HashSet<&str>) -> TokenStream {
    let method_tokens: Vec<TokenStream> = methods
        .iter()
        .filter(|m| m.is_async && !SKIP_METHODS.contains(&m.name.as_str()))
        .map(|m| emit_method(m, enum_names))
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

fn emit_method(m: &MethodDetails, enum_names: &HashSet<&str>) -> TokenStream {
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

    // Build JSDoc lines.
    let mut doc_lines: Vec<String> = Vec::new();
    for p in &m.params {
        let js_ty = type_map::param_js_type(&p.ty, enum_names);
        let optional = matches!(p.ty, super::super::RustType::Option(_));
        let name_str = if optional {
            format!("[{}]", p.name)
        } else {
            p.name.clone()
        };
        doc_lines.push(format!("@param {{{js_ty}}} {name_str}"));
    }
    let ret_js = match &m.return_type {
        Some(ty) => type_map::return_js_type(ty, enum_names),
        None => "Promise<void>".to_string(),
    };
    doc_lines.push(format!("@returns {{{ret_js}}}"));

    let doc_attrs: Vec<TokenStream> = doc_lines
        .iter()
        .map(|line| {
            quote! { #[doc = #line] }
        })
        .collect();

    quote! {
        #(#doc_attrs)*
        #js_attr
        pub async fn #method(&self, #(#wasm_params),*) -> #ret_ty {
            #body
        }
    }
}
