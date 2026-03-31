//! Emit namespace structs (User, Public, Admin, Keeper, Vault) with factory methods.

use heck::{ToLowerCamelCase, ToSnakeCase};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::super::{ActionGroup, VariantInfo};
use super::field_assignments;

fn namespace_doc(name: &str) -> &'static str {
    match name {
        "User" => "User trading operations (deposit, withdraw, orders, vaults, etc.).",
        "Public" => "Permissionless operations anyone can call (liquidations, funding, etc.).",
        "Keeper" => "Keeper-only operations (oracle prices, funding rates, fee tiers, etc.).",
        "Vault" => "Vault leader operations (config, withdrawals, delegation, etc.).",
        "Admin" => "Admin-only operations (market init, global config, force actions, etc.).",
        _ => "Exchange operations.",
    }
}

pub fn emit_namespace(group: &ActionGroup) -> TokenStream {
    let ns = format_ident!("{}", group.call_message_variant);
    let doc = namespace_doc(&group.call_message_variant);

    let methods: Vec<TokenStream> = group
        .variants
        .iter()
        .map(|v| emit_factory(group, v))
        .collect();

    quote! {
        #[doc = #doc]
        #[wasm_bindgen]
        pub struct #ns;

        #[wasm_bindgen]
        impl #ns {
            #(#methods)*
        }
    }
}

fn emit_factory(group: &ActionGroup, variant: &VariantInfo) -> TokenStream {
    let rust_fn_name = format_ident!("{}", variant.variant_name.to_snake_case());
    let js_name = variant.variant_name.to_lower_camel_case();
    let action_enum = format_ident!("{}", group.action_enum);
    let cm_variant = format_ident!("{}", group.call_message_variant);
    let variant_name = format_ident!("{}", variant.variant_name);

    // Sort: required first, optional last.
    let mut field_order: Vec<usize> = (0..variant.fields.len()).collect();
    field_order.sort_by_key(|&i| variant.fields[i].is_optional as u8);

    let params: Vec<TokenStream> = field_order
        .iter()
        .map(|&i| {
            let name = format_ident!("{}", variant.fields[i].name);
            let ty: TokenStream = variant.fields[i]
                .param_type
                .parse()
                .expect("param type should parse");
            quote! { #name: #ty }
        })
        .collect();

    let assignments = field_assignments(&variant.fields);

    let body = if assignments.is_empty() {
        quote! {
            Ok(WasmCallMessage {
                inner: CallMessage::#cm_variant(#action_enum::#variant_name {}),
            })
        }
    } else {
        quote! {
            Ok(WasmCallMessage {
                inner: CallMessage::#cm_variant(#action_enum::#variant_name {
                    #(#assignments),*
                }),
            })
        }
    };

    quote! {
        #[wasm_bindgen(js_name = #js_name)]
        #[allow(clippy::too_many_arguments)]
        pub fn #rust_fn_name(#(#params),*) -> WasmResult<WasmCallMessage> {
            #body
        }
    }
}
