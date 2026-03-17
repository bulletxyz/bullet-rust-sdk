//! Emit wasm-bindgen C-style enum wrappers with `into_domain()` conversion.

use super::super::SchemaEnum;

pub fn emit_enum(e: &SchemaEnum) -> String {
    let type_name = &e.type_name;
    let wrapper_name = format!("Wasm{type_name}");

    let mut out = String::new();

    // C-style wasm_bindgen enum.
    out.push_str(&format!("#[wasm_bindgen(js_name = {type_name})]\n"));
    out.push_str("#[derive(Clone, Copy)]\n");
    out.push_str(&format!("pub enum {wrapper_name} {{\n"));
    for (i, variant) in e.variants.iter().enumerate() {
        out.push_str(&format!("    {variant} = {i},\n"));
    }
    out.push_str("}\n\n");

    // into_domain() conversion.
    out.push_str(&format!("impl {wrapper_name} {{\n"));
    out.push_str(&format!("    fn into_domain(self) -> {type_name} {{\n"));
    out.push_str("        match self {\n");
    for variant in &e.variants {
        out.push_str(&format!(
            "            {wrapper_name}::{variant} => {type_name}::{variant},\n"
        ));
    }
    out.push_str("        }\n");
    out.push_str("    }\n");
    out.push_str("}\n");

    out
}
