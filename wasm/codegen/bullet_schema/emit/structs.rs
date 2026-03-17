//! Emit wasm-bindgen struct wrappers with typed constructors.

use std::collections::HashSet;

use super::super::map;
use super::super::{SchemaStruct, Types};
use super::field_assignments;

pub fn emit_struct(
    s: &SchemaStruct,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> String {
    let type_name = &s.type_name;
    let wrapper_name = format!("Wasm{type_name}");
    let js_name = type_name;

    // Some types are generic over Address.
    let inner_type_decl = match type_name.as_str() {
        "CreateVaultArgs" | "UpdateGlobalConfigArgs" => format!("{type_name}<Address>"),
        _ => type_name.clone(),
    };
    let inner_type_init = type_name;

    let mappings = map::map_fields(&s.fields, types, wrapper_indices, enum_indices);

    // Sort: required params first, optional last.
    let mut field_order: Vec<usize> = (0..s.fields.len()).collect();
    field_order.sort_by_key(|&i| mappings[i].is_optional as u8);

    let params: Vec<String> = field_order
        .iter()
        .map(|&i| format!("{}: {}", s.fields[i].name, mappings[i].param_type))
        .collect();

    let assignments = field_assignments(&s.fields, &mappings);

    let mut out = String::new();

    // Struct definition.
    out.push_str(&format!(
        "/// Wrapper for `{type_name}`.\n\
         #[wasm_bindgen(js_name = {js_name})]\n\
         pub struct {wrapper_name} {{\n\
         \x20   pub(crate) inner: {inner_type_decl},\n\
         }}\n\n"
    ));

    // Constructor impl.
    out.push_str(&format!(
        "#[wasm_bindgen(js_class = {js_name})]\n\
         impl {wrapper_name} {{\n\
         \x20   #[wasm_bindgen(constructor)]\n\
         \x20   pub fn new({params}) -> WasmResult<{wrapper_name}> {{\n\
         \x20       Ok({wrapper_name} {{\n\
         \x20           inner: {inner_type_init} {{\n\
         {assignments}\n\
         \x20           }},\n\
         \x20       }})\n\
         \x20   }}\n\
         }}\n",
        params = params.join(", "),
    ));

    out
}
