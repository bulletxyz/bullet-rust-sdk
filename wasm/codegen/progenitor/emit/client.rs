//! Emit wasm-bindgen client method wrappers.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::{MethodInfo, ParamInfo, ParamType, ReturnKind};

/// Emit the full `impl WasmTradingApi` block.
pub fn emit_client_impl(methods: &[MethodInfo]) -> TokenStream {
    let method_tokens: Vec<TokenStream> = methods.iter().map(emit_method).collect();

    quote! {
        use crate::client::WasmTradingApi;
        use crate::errors::WasmResult;

        #[wasm_bindgen(js_class = Client)]
        impl WasmTradingApi {
            #(#method_tokens)*
        }
    }
}

fn emit_method(m: &MethodInfo) -> TokenStream {
    let method = format_ident!("{}", m.name);
    let js_name = to_camel_case(&m.name);

    // Build the WASM-facing parameter list.
    let wasm_params: Vec<TokenStream> = m.params.iter().map(wasm_param).collect();

    // Build the inner call arguments (same order as progenitor, which is the order we parsed).
    let call_args: Vec<TokenStream> = m.params.iter().map(call_arg).collect();

    // Return type + response handling.
    let (ret_ty, body) = match &m.ret {
        ReturnKind::Schema(name) => {
            let w = format_ident!("Wasm{}", name);
            (
                quote! { WasmResult<#w> },
                quote! { Ok(#w(self.inner.#method(#(#call_args),*).await?.into_inner())) },
            )
        }
        ReturnKind::Array(name) => {
            let w = format_ident!("Wasm{}", name);
            (
                quote! { WasmResult<Vec<#w>> },
                quote! {
                    Ok(self.inner.#method(#(#call_args),*).await?.into_inner()
                        .into_iter().map(#w).collect())
                },
            )
        }
        ReturnKind::Stream => (
            quote! { WasmResult<String> },
            quote! {
                use futures_util::TryStreamExt as _;
                let bytes: Vec<u8> = self.inner.#method(#(#call_args),*).await?.into_inner().into_inner()
                    .map_ok(|b| b.to_vec())
                    .try_concat()
                    .await?;
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            },
        ),
        ReturnKind::Unit => (
            quote! { WasmResult<()> },
            quote! {
                self.inner.#method(#(#call_args),*).await?;
                Ok(())
            },
        ),
        ReturnKind::JsonMap => (
            quote! { WasmResult<String> },
            quote! {
                Ok(serde_json::to_string(&self.inner.#method(#(#call_args),*).await?.into_inner())?)
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

/// WASM-facing parameter declaration.
fn wasm_param(p: &ParamInfo) -> TokenStream {
    let name = format_ident!("{}", p.name);
    match &p.ty {
        ParamType::Str => quote! { #name: &str },
        ParamType::OptionStr => quote! { #name: Option<String> },
        ParamType::I32 => quote! { #name: i32 },
        ParamType::OptionI32 => quote! { #name: Option<i32> },
        ParamType::I64 => quote! { #name: i64 },
        ParamType::OptionI64 => quote! { #name: Option<i64> },
        ParamType::BodyRef(ty_name) => {
            let w = format_ident!("Wasm{}", ty_name);
            quote! { #name: &#w }
        }
    }
}

/// Argument expression passed to the inner client method.
fn call_arg(p: &ParamInfo) -> TokenStream {
    let name = format_ident!("{}", p.name);
    match &p.ty {
        ParamType::Str => quote! { #name },
        ParamType::OptionStr => quote! { #name.as_deref() },
        ParamType::I32 | ParamType::I64 => quote! { #name },
        ParamType::OptionI32 | ParamType::OptionI64 => quote! { #name },
        ParamType::BodyRef(_) => quote! { &#name.0 },
    }
}

/// Convert `snake_case` to `camelCase`.
fn to_camel_case(s: &str) -> String {
    let mut result = String::new();
    let mut cap_next = false;
    for c in s.chars() {
        if c == '_' {
            cap_next = true;
        } else if cap_next {
            result.push(c.to_ascii_uppercase());
            cap_next = false;
        } else {
            result.push(c);
        }
    }
    result
}
