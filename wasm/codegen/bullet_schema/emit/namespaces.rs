//! Emit namespace structs (User, Public, Admin, Keeper, Vault) with factory methods.

use std::collections::HashSet;

use heck::{ToLowerCamelCase, ToSnakeCase};

use super::super::map;
use super::super::{ActionGroup, Types, VariantInfo};
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

pub fn emit_namespace(
    group: &ActionGroup,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> String {
    let ns = &group.call_message_variant;

    let mut out = String::new();

    out.push_str(&format!("/// {}\n", namespace_doc(ns)));
    out.push_str("#[wasm_bindgen]\n");
    out.push_str(&format!("pub struct {ns};\n\n"));

    out.push_str("#[wasm_bindgen]\n");
    out.push_str(&format!("impl {ns} {{\n"));

    for variant in &group.variants {
        out.push_str(&emit_factory(
            group,
            variant,
            types,
            wrapper_indices,
            enum_indices,
        ));
    }

    out.push_str("}\n");
    out
}

fn emit_factory(
    group: &ActionGroup,
    variant: &VariantInfo,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> String {
    let mappings = map::map_fields(&variant.fields, types, wrapper_indices, enum_indices);

    let rust_fn_name = variant.variant_name.to_snake_case();
    let js_name = variant.variant_name.to_lower_camel_case();
    let action_enum = &group.action_enum;
    let cm_variant = &group.call_message_variant;
    let variant_name = &variant.variant_name;

    // Sort: required first, optional last.
    let mut field_order: Vec<usize> = (0..variant.fields.len()).collect();
    field_order.sort_by_key(|&i| mappings[i].is_optional as u8);

    let params: Vec<String> = field_order
        .iter()
        .map(|&i| format!("{}: {}", variant.fields[i].name, mappings[i].param_type))
        .collect();

    let assignments = field_assignments(&variant.fields, &mappings);

    let body = if assignments.is_empty() {
        format!(
            "        Ok(WasmCallMessage {{\n\
             \x20           inner: CallMessage::{cm_variant}({action_enum}::{variant_name} {{}}),\n\
             \x20       }})"
        )
    } else {
        format!(
            "        Ok(WasmCallMessage {{\n\
             \x20           inner: CallMessage::{cm_variant}({action_enum}::{variant_name} {{\n\
             {assignments}\n\
             \x20           }}),\n\
             \x20       }})"
        )
    };

    format!(
        "    #[wasm_bindgen(js_name = {js_name})]\n\
         \x20   pub fn {rust_fn_name}({params}) -> WasmResult<WasmCallMessage> {{\n\
         {body}\n\
         \x20   }}\n\n",
        params = params.join(", "),
    )
}
